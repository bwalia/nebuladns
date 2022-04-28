<template>
  <div class="form">
    <form class="form-signin" @submit.prevent="handleRegister" @onkeydown="clearError">
      <h1 class="h3 mb-3 font-weight-normal">Sign Up</h1>
      <div class="row col-12 form-group">
      <label for="inputFirstName" class="sr-only">First Name</label>
      <input
        id="inputFirstName"
        v-model="first_name"
        type="text"
        class="form-control"
        placeholder="First Name"
        autofocus
        :class="{ 'is-invalid': firstNameError }"
        @keydown="firstNameError = ''"
      />
      <div v-if="firstNameError" class="invalid-feedback" :state="firstNameError">{{ firstNameError }}</div>
      </div>
      <div class="row col-12 form-group">
      <label for="inputLastName" class="sr-only">Last Name</label>
      <input
        id="inputLastName"
        v-model="last_name"
        type="text"
        class="form-control"
        placeholder="Last Name"
        autofocus
        :class="{ 'is-invalid': lastNameError }"
        @keydown="lastNameError = ''"
      />
      <div v-if="lastNameError" class="invalid-feedback" :state="lastNameError">{{ lastNameError }}</div>
      </div>
      <div class="row col-12 form-group">
      <label for="inputEmail" class="sr-only">Email address</label>
      <input
        id="inputEmail"
        v-model="email"
        type="email"
        class="form-control"
        placeholder="Email address"
        autofocus
        :class="{ 'is-invalid': emailError }"
        @keydown="emailError = ''"
      />
      <div v-if="emailError" class="invalid-feedback" :state="emailError">{{ emailError }}</div>
      </div>
      <div class="row col-12 form-group">
      <label for="inputPassword" class="sr-only">Password</label>
      <input
        id="inputPassword"
        v-model="password"
        type="password"
        class="form-control"
        placeholder="Password"
        :class="{ 'is-invalid': passwordError }"
        @keydown="passwordError = ''"
      />
       <div v-if="passwordError" class="invalid-feedback" :state="passwordError">{{ passwordError }}</div>
      </div>
      <div class="checkbox mb-3">
        <label>
          <nuxt-link to="/login"> Log In </nuxt-link>
        </label>
      </div>
      <button class="row col-6 btn btn-lg btn-primary ml-1" type="submit">
        Sign Up
      </button>
    </form>
  </div>
</template>

<script>
export default {
  data() {
    return {
      email: "",
      password: "",
      last_name: "",
      first_name: "",
      firstNameError: "",
      lastNameError: "",
      emailError: "",
      passwordError: "",
    }
  },
  methods: {
    handleRegister() {
      if(this.checkErro()===false)
      {
        this.$axios
          .post(
            'https://gorest.co.in/public/v2/users',
            {
              email: this.email,
              password: this.password,
              name: this.first_name,
            },
            { headers: { 'Content-Type': 'application/json' } }
          )
          .then((res) => {
            this.$store.commit('auth/setToken', res.data.token)
            this.$cookies.set('token', res.data.token)
            this.$router.push('/dashboard')
          })
          .catch((e) => {
           console.log('catch');
           console.log(e.response.data.message);
            this.$toast.error(e.response.data._message,{timeout:2000});
          })
      }
      else{
        console.log('Error');
      }
    },
    validateEmail(email) 
    {
      const data=email.split("@");
      if(data.length<2) return false;
      const dot=data[1].split(".");
      if(dot.length<2) return false;
      if(email.indexOf('@')>0 && email.indexOf('.')>0) return true;
      return false;
    }
    ,
    checkErro(){
      if(this.emailError.length===0 && this.passwordError.length===0 && this.validateEmail(this.email)===true && this.email.length>0 && this.password.length>5 && this.firstNameError.length===0 && this.first_name.length>0 && this.lastNameError.length===0 && this.last_name.length>0)
      {
        return false;
      }
      else
      {
        if(this.email.length===0)
        {
          this.emailError ="Email is Required";
        }
        else if(this.validateEmail(this.email)===false)
        {
          this.emailError ="Email is not valid";
        }
        if(this.password.length===0)
        {
          this.passwordError ="Password is Required";
        }
        else if(this.password.length<6)
        {
          this.passwordError ="Password Must be at least 6 character";
        }
        if(this.first_name.length===0)
        {
          this.firstNameError ="First Name is Required";
        }
        if(this.last_name.length===0)
        {
          this.lastNameError ="Last Name is Required";
        }
        return true;
      }
    },
    clearError(){
      this.emailError="";
      this.passwordError="";
    },
  },
}
</script>

<style scoped>
.form {
  height: 100vh;
  display: -ms-flexbox;
  display: flex;
  -ms-flex-align: center;
  align-items: center;
  padding-top: 40px;
  padding-bottom: 40px;
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


</style>
