<template>
  <main class="main">
    <div class="container">
      <logo class="odin-logo" />
      <section class="wrapper">
        <div class="heading">
          <h1 class="text text-large">Customer Login</h1>
        </div>
        <div class="form">
          <form class="form-signin" @submit.prevent="handleLogin" @onkeydown="clearError">
            <div class="form-group input-control">
              <label for="inputEmail" class="sr-only input-label" hidden>Email address</label>
              <input id="inputEmail" v-model="email" name="email" type="email" class="form-control input-field"
                placeholder="Email address" autofocus :class="{ 'is-invalid': emailError }"
                @keydown="emailError = ''" />
              <div v-if="emailError" class="invalid-feedback" :state="emailError">{{ emailError }}</div>
            </div>


            <!-- <div class="u-shim-t"> -->
            <div class="form-group input-control">
              <label for="inputPassword" class="sr-only input-label" hidden>Password</label><br />
              <input id="inputPassword" v-model="password" type="password" name="password"
                class="form-control input-field" placeholder="Password" :class="{ 'is-invalid': passwordError }"
                @keydown="passwordError = ''" />
              <div v-if="passwordError" class="invalid-feedback" :state="passwordError">{{ passwordError }}</div>
            </div>
            <!-- </div> -->

            <!-- <div class="checkbox mb-3">
        <label>
          <nuxt-link to="/signup"> Get Register </nuxt-link>
        </label>
      </div> -->
          <div class="button-wrapper">
            <a href="/" class="u-shim-t btn btn-back">Go Back</a>
            <button class="u-button u-shim-t btn input-submit" type="submit">
              Log-in
            </button>
          </div>

          </form>

        </div>
      </section>
    </div>
  </main>
</template>

<script>
export default {
  layout: 'blank',
  mounted() {
    console.log({currentENV: process.env.NODE_ENV});
    // console.log({"baseAPIURL public runtime config": this.$config.baseAPIURL});
    console.log({ "apiSecretPub private runtime config": this.$config.apiSecretPub });

    if (this.$auth.$storage.getUniversal('loggedIn')) {
      this.$router.push('/clients')
    }
  },
  data() {
    return {
      email: "",
      password: "",
      emailError: "",
      passwordError: "",
    }
  },

  methods: {
    handleLogin() {
      // if ANY fail validation
      if (this.checkErro() === false) {
        this.$axios
          .post(
            process.env.basePubURL + '/auth/login',
            {
              email: this.email,
              password: this.password
            },
            {
              headers: {
                'Access-Control-Allow-Origin': '*',
                'Content-Type': 'application/Json'
              }
            }
          )
          .then((res) => {
            if (res.data.access_token) {
              this.$auth.$storage.setUniversal('token', res.data.access_token);
              this.$auth.$storage.setUniversal('name', res.data.user.name);
              this.$auth.$storage.setUniversal('id', res.data.user.id);
              this.$auth.$storage.setUniversal('loggedIn', true);
              this.$auth.$storage.setUniversal('user', res.data.user);
              this.$toast.success('Successfully authenticated', { position: 'bottom-center', duration:2000 });
              this.$router.push('/clients');
            }
            else {
              this.$toast.error(res.data.error, { position: 'bottom-center', duration:2000 });
            }

          })
          .catch((e) => {
            this.$toast.error(e.response.data.error, { position: 'bottom-center', duration:2000 });
            // this.$toast.show('Logging in...')
            console.log(e.response, "106");
          })
      }
    },
    validateEmail(email) {
      const data = email.split("@");
      if (data.length < 2) return false;
      const dot = data[1].split(".");
      if (dot.length < 2) return false;
      if (email.indexOf('@') > 0 && email.indexOf('.') > 0) return true;
      return false;
    }
    ,
    checkErro() {
      if (this.emailError.length === 0 && this.passwordError.length === 0 && this.validateEmail(this.email) === true && this.password.length > 5 && this.email.length > 0) {
        return false;
      }
      else {
        if (this.email.length === 0) {
          this.emailError = "Email is Required";
        }
        else if (this.validateEmail(this.email) === false) {
          this.emailError = "Email is not valid";
        }
        if (this.password.length === 0) {
          this.passwordError = "Password is Required";
        }
        else if (this.password.length < 6) {
          this.passwordError = "Password Must be at least 6 character";
        }
        return true;
      }
    },
    clearError() {
      this.emailError = "";
      this.passwordError = "";
    }
  }
}
</script>

<!-- <style scoped>
.form {
  height: 100vh;
  display: -ms-flexbox;
  display: flex;
  -ms-flex-align: center;
  align-items: center;
  padding-top: 30px;
  padding-bottom: 50px;
  background-color: #f5f5f5;
}
.form-signin {
  width: 100%;
  max-width: 330px;
  padding: 15px;
  margin: auto;
}
.form-signin .checkbox {
  font-weight: 400;
}
.form-signin .form-control {
  position: relative;
  box-sizing: border-box;
  height: auto;
  padding: 10px;
  font-size: 16px;
}

.form-signin input {
  width: 100%;
  max-width: 300px;
}
.form-signin .form-control:focus {
  z-index: 2;
}
.form-signin input[type='email'] {
  margin-bottom: -1px;
  border-bottom-right-radius: 0;
  border-bottom-left-radius: 0;
}
.form-signin input[type='password'] {
  margin-bottom: 10px;
  border-top-left-radius: 0;
  border-top-right-radius: 0;
}
.bd-placeholder-img {
  font-size: 1.125rem;
  text-anchor: middle;
  -webkit-user-select: none;
  -moz-user-select: none;
  -ms-user-select: none;
  user-select: none;
}
div:focus {
    background-color: red;
}


</style> -->
